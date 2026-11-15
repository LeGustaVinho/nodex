use nodex_lang_rust::lower_source;

#[test]
fn derives_explain_declared_helpers_without_claiming_macro_behavior() {
    let source = "#[proc_macro_derive(Name, attributes(helper, other))] #[inline] pub fn derive_name(input: TokenStream) -> TokenStream { input } #[proc_macro_derive(Simple)] pub fn simple(input: TokenStream) -> TokenStream { input } #[custom(attributes(helper))] pub fn other() {}";
    let ir = lower_source("src/lib.rs", source);
    let symbol = ir.symbols.iter().find(|s| s.name == "derive_name").unwrap();
    let items: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter().filter(|i| i.group == "Derive helper attributes").collect();
    assert_eq!(items.len(), 1, "{:?}", symbol.rust_contract);
    assert_eq!(&source[items[0].span.start_byte as usize..items[0].span.end_byte as usize], "(helper, other)");
    for part in ["#[helper]", "#[other]", "derive macro Name", "after the derive", "inert", "other macros", "depend on macro code"] { assert!(items[0].text.contains(part), "{}", items[0].text); }
    assert!(symbol.rust_contract.as_ref().unwrap().iter().any(|i| i.group == "Derive procedural macro"));
    for name in ["simple", "other"] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|i| i.group == "Derive helper attributes"));
    }
}
