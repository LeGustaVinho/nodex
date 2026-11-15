use nodex_lang_rust::lower_source;

#[test]
fn zero_or_more_comma_separated_expr_fragments_are_explained_only_in_matchers() {
    let source = r#"macro_rules! m {
        ($($x:expr),*) => { stringify!($($x),*) };
        ($($y:expr),* $(,)?) => { stringify!($($y),*) };
        ($( ($($inner:expr),*) );*) => { () };
        ($(pre $z:expr),*) => { () };
        ($($semi:expr);*) => { () };
        ($($plus:expr),+) => { () };
        ($id:ident) => { stringify!($($fake:expr),*) };
    }"#;
    let ir = lower_source("src/lib.rs", source);
    let symbol = ir.symbols.iter().find(|symbol| symbol.name == "m!").unwrap();
    let items: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Comma-separated expression repetition").collect();
    assert_eq!(items.len(), 3, "{:?}", symbol.rust_contract);
    for variable in ["$x:expr", "$y:expr", "$inner:expr"] {
        let item = items.iter().find(|item| item.text.contains(variable)).unwrap_or_else(|| panic!("{variable}: {items:?}"));
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "*");
        assert!(item.text.contains("zero or more") && item.text.contains("comma separates")
            && item.text.contains("does not itself evaluate") && item.text.contains("trailing comma")
            && item.text.contains("edition"));
    }
}
