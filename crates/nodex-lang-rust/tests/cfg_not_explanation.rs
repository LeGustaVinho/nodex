use nodex_lang_rust::lower_source;

#[test]
fn cfg_not_reverses_one_predicate_without_guessing_active_configuration() {
    let source = "#[cfg(not(a))] #[inline] fn f() {} #[cfg(not(any(a, b)))] struct S; #[cfg(not(feature = \"x\"))] fn feature() {} #[cfg(all(not(a), b))] fn nested() {} #[cfg_attr(not(a), inline)] fn indirect() {} #[cfg(not(a, b))] fn invalid() {}";
    let ir = lower_source("src/lib.rs", source);
    for name in ["f", "S", "feature"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Negated configuration predicate").unwrap_or_else(|| panic!("{name}: {:?}", symbol.rust_contract));
        assert!(source[item.span.start_byte as usize..item.span.end_byte as usize].starts_with("#[cfg(not("));
        assert!(item.text.contains("reverses") && item.text.contains("when that predicate is true") && item.text.contains("subject to any other conditions") && item.text.contains("does not establish"));
    }
    for name in ["nested", "indirect", "invalid"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Negated configuration predicate"), "{name}");
    }
}
