use nodex_lang_rust::lower_source;

#[test]
fn must_use_reason_is_preserved_in_contract_for_function_type_and_trait() {
    let source = r##"#[must_use = "check this result"] #[inline] fn calculate() -> i32 { 1 }
        #[must_use = r#"keep \\ exact"#] struct Guard;
        #[must_use = "poll it"] trait Work {}
        #[must_use] fn bare() {}
        #[cfg_attr(feature = "x", must_use = "conditional")] fn indirect() {}
        #[other = "not must_use"] fn other() {}"##;
    let ir = lower_source("src/lib.rs", source);
    for (name, reason) in [("calculate", "\"check this result\""), ("Guard", "r#\"keep \\\\ exact\"#"), ("Work", "\"poll it\"")] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Must-use reason").unwrap_or_else(|| panic!("{name}: {:?}", symbol.rust_contract));
        assert!(item.text.contains(reason), "{}", item.text);
        assert!(item.text.contains("unused_must_use") && item.text.contains("may include") && item.text.contains("lint settings") && item.text.contains("at runtime"));
        assert!(source[item.span.start_byte as usize..item.span.end_byte as usize].starts_with("#[must_use ="));
    }
    for name in ["bare", "indirect", "other"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Must-use reason"), "{name}");
    }
}
