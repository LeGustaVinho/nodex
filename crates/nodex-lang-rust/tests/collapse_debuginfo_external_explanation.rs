use nodex_lang_rust::lower_source;

#[test]
fn explains_external_macro_debug_locations_and_override() {
    let source = "#[collapse_debuginfo(external)] #[allow(unused)] /// docs\n macro_rules! example { () => {} } #[collapse_debuginfo(yes)] macro_rules! yes { () => {} } #[cfg_attr(feature = \"x\", collapse_debuginfo(external))] macro_rules! conditional { () => {} } #[collapse_debuginfo(external)] fn ordinary() {}";
    let ir = lower_source("src/lib.rs", source);
    let example = ir.symbols.iter().find(|symbol| symbol.name == "example!").unwrap();
    let item = example.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Macro debug locations for external callers").unwrap();
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "#[collapse_debuginfo(external)]");
    for expected in ["different crate", "call site", "own crate", "not what the macro executes", "-C collapse-macro-debuginfo", "override"] {
        assert!(item.text.contains(expected), "missing {expected}");
    }
    for name in ["yes!", "conditional!", "ordinary"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Macro debug locations for external callers"), "{name}");
    }
}
