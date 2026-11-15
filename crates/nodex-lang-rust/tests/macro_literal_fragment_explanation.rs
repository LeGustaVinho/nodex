use nodex_lang_rust::lower_source;

#[test]
fn literal_fragments_are_explained_in_matchers_including_repetition_but_not_transcribers() {
    let source = r#"macro_rules! m {
        ($x:literal) => { stringify!($x) };
        ($( $value:literal ),*) => { stringify!($( $value ),*) };
        ($id:ident) => { stringify!($id:literal) };
    }"#;
    let ir = lower_source("src/lib.rs", source);
    let symbol = ir.symbols.iter().find(|symbol| symbol.name == "m!").unwrap();
    let items: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Literal macro fragment").collect();
    assert_eq!(items.len(), 2, "{:?}", symbol.rust_contract);
    let x = items.iter().find(|item| item.text.contains("$x:literal")).unwrap();
    let repeated = items.iter().find(|item| item.text.contains("$value:literal")).unwrap();
    for item in &items {
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "literal");
        assert!(item.text.contains("optionally preceded by -") && item.text.contains("not a runtime function argument")
            && item.text.contains("suffixes") && item.text.contains("not an ordinary Rust variable"));
    }
    assert!(!x.text.contains("inside a repetition"));
    assert!(repeated.text.contains("inside a repetition") && repeated.text.contains("each matched iteration"));
}
