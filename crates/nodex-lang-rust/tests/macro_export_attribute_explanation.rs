use nodex_lang_rust::lower_source;

#[test]
fn exported_macro_is_explained_at_its_nested_definition() {
    let source = "mod nested { #[macro_export] #[allow(unused)] /// Docs\n macro_rules! hello { () => {} } #[macro_export(local_inner_macros)] macro_rules! helper { () => {} } macro_rules! local { () => {} } #[cfg_attr(feature = \"x\", macro_export)] macro_rules! conditional { () => {} } } #[macro_export] fn not_macro() {}";
    let ir = lower_source("src/lib.rs", source);
    for name in ["hello!", "helper!"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Export a declarative macro").unwrap_or_else(|| panic!("{name}: {:?}", symbol.rust_contract));
        assert!(source[item.span.start_byte as usize..item.span.end_byte as usize].starts_with("#[macro_export"));
        assert!(item.text.contains("crate root") && item.text.contains("inside a module") && item.text.contains("automatically import") && item.text.contains("Conditional compilation"));
        if name == "helper!" { assert!(item.text.contains("local_inner_macros") && item.text.contains("$crate::")); }
    }
    for name in ["local!", "conditional!", "not_macro"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Export a declarative macro"), "{name}");
    }
}
