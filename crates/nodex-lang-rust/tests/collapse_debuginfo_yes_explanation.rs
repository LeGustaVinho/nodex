use nodex_lang_rust::lower_source;

#[test]
fn explains_yes_on_macro_and_excludes_other_options_and_items() {
    let source = "#[collapse_debuginfo(yes)] macro_rules! yes_macro { () => {} } #[collapse_debuginfo(no)] macro_rules! no_macro { () => {} } #[collapse_debuginfo(external)] macro_rules! ext_macro { () => {} } #[collapse_debuginfo(yes)] fn invalid_target() {}";
    let file = lower_source("src/lib.rs", source);
    let symbol = file.symbols.iter().find(|symbol| symbol.name == "yes_macro!").unwrap();
    let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Collapse macro debug locations").unwrap_or_else(|| panic!("{:?}", symbol.rust_contract));
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "#[collapse_debuginfo(yes)]");
    assert!(item.text.contains("call-site"));
    assert!(item.text.contains("different crate"));
    assert!(item.text.contains("can override"));
    assert!(item.text.contains("does not change"));
    for name in ["no_macro!", "ext_macro!", "invalid_target"] {
        let other = file.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!other.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Collapse macro debug locations"));
    }
}
