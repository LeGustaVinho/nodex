use nodex_lang_rust::lower_source;
#[test]
fn procedural_attribute_macro_is_explained_on_the_annotated_function_only() {
    let source = "#[proc_macro_attribute] // note\n#[allow(dead_code)] pub fn pass(args: TokenStream, item: TokenStream) -> TokenStream { item } fn plain() {} #[other(proc_macro_attribute)] fn other() {} #[proc_macro_derive(X)] pub fn derive(input: TokenStream) -> TokenStream { input }";
    let ir = lower_source("src/lib.rs", source);
    let function = ir.symbols.iter().find(|symbol| symbol.name == "pass").unwrap();
    let items: Vec<_> = function.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Procedural attribute macro").collect();
    assert_eq!(items.len(), 1, "{:?}", function.rust_contract);
    let item = items[0];
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "#[proc_macro_attribute]");
    assert!(item.text.contains("two token streams") && item.text.contains("arguments") && item.text.contains("replaces the annotated item") && item.text.contains("during compilation") && item.text.contains("does not say what transformation"));
    for name in ["plain", "other", "derive"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Procedural attribute macro"));
    }
}
