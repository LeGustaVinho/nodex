use nodex_lang_rust::lower_source;

#[test]
fn derive_registration_is_explained_on_the_implementing_function() {
    let source = "#[proc_macro_derive(Pretty, attributes(helper))] #[inline] pub fn derive_pretty(input: TokenStream) -> TokenStream { input } fn plain(input: TokenStream) -> TokenStream { input } #[proc_macro_attribute] pub fn attr(x: TokenStream, y: TokenStream) -> TokenStream { x }";
    let ir = lower_source("src/lib.rs", source);
    let symbol = ir.symbols.iter().find(|s| s.name == "derive_pretty").unwrap();
    let facts: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Derive procedural macro").collect();
    assert_eq!(facts.len(), 1, "{:?}", symbol.rust_contract);
    assert_eq!(&source[facts[0].span.start_byte as usize..facts[0].span.end_byte as usize], "#[proc_macro_derive(Pretty, attributes(helper))]");
    assert!(facts[0].text.contains("#[derive(Pretty)]") && facts[0].text.contains("derive_pretty") && facts[0].text.contains("additional items") && facts[0].text.contains("does not prove"));
    for name in ["plain", "attr"] {
        let other = ir.symbols.iter().find(|s| s.name == name).unwrap();
        assert!(!other.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Derive procedural macro"));
    }
}
