use nodex_lang_rust::lower_source;

#[test]
fn explains_optional_token_tree_repetition_in_matchers_only() {
    let source = "macro_rules! m { ($($x:tt)?) => { $($x)? }; ($($y:tt)*) => { $($y)* }; ($a:ident) => { $($z:tt)? }; }";
    let ir = lower_source("src/lib.rs", source);
    let symbol = ir.symbols.iter().find(|symbol| symbol.name == "m!").unwrap();
    let items: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Optional macro token tree").collect();
    assert_eq!(items.len(), 1, "{:?}", symbol.rust_contract);
    let item = items[0];
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "$($x:tt)?");
    assert!(item.text.contains("zero or one") && item.text.contains("balanced group") && item.text.contains("not a runtime condition") && item.text.contains("no repetition separator"));
}
