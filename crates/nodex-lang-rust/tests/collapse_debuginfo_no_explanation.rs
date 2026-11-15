use nodex_lang_rust::lower_source;

#[test]
fn collapse_debuginfo_no_explains_macro_locations_without_claiming_debugger_behavior() {
    let source = "#[collapse_debuginfo(no)] macro_rules! m { () => { 1 }; } #[collapse_debuginfo(yes)] macro_rules! yes { () => { 2 }; } #[collapse_debuginfo(external)] macro_rules! ext { () => { 3 }; } #[cfg_attr(feature = \"x\", collapse_debuginfo(no))] macro_rules! conditional { () => { 4 }; } #[collapse_debuginfo(no)] fn wrong_position() {}";
    let ir = lower_source("src/lib.rs", source);
    let m = ir.symbols.iter().find(|symbol| symbol.name == "m!").unwrap();
    let item = m.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Macro debug-location mapping").expect("macro contract");
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "#[collapse_debuginfo(no)]");
    assert!(item.text.contains("not be collapsed"));
    assert!(item.text.contains("does not change macro matching"));
    assert!(item.text.contains("can override"));
    assert!(item.text.contains("does not prove debug information is emitted"));
    for name in ["yes!", "ext!", "conditional!", "wrong_position"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Macro debug-location mapping"), "{name}: {:?}", symbol.rust_contract);
    }
}
