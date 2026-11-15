use nodex_lang_rust::lower_source;

#[test]
fn proc_macro_attribute_is_explained_on_the_function_not_other_attributes() {
    let source = "#[proc_macro]\n#[allow(dead_code)]\n/// Documentation\npub fn make(input: TokenStream) -> TokenStream { input }\n#[proc_macro_attribute] pub fn attribute(args: TokenStream, item: TokenStream) -> TokenStream { item }\n#[proc_macro_derive(Thing)] pub fn derive(input: TokenStream) -> TokenStream { input }\nfn ordinary() {}";
    let ir = lower_source("src/lib.rs", source);
    let symbol = ir.symbols.iter().find(|symbol| symbol.name == "make").unwrap();
    let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Function-like procedural macro").expect("macro explanation");
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "#[proc_macro]");
    assert!(item.text.contains("make!(...)") && item.text.contains("tokens inside") && item.text.contains("replaces the invocation"));
    assert!(item.text.contains("proc-macro") && item.text.contains("does not prove"));
    for name in ["attribute", "derive", "ordinary"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Function-like procedural macro"), "{name}");
    }
}
