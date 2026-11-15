use nodex_lang_rust::lower_source;

#[test]
fn explains_lifetime_matcher_including_repetition_but_not_transcriber() {
    let source = "macro_rules! lifetimes { ($lt:lifetime) => { $lt }; ($( $item:lifetime ),*) => { $( $item )* }; ($x:tt) => { $x:lifetime }; }";
    let ir = lower_source("src/lib.rs", source);
    let symbol = ir.symbols.iter().find(|symbol| symbol.name == "lifetimes!").unwrap();
    let items: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter()
        .filter(|item| item.group == "Macro lifetime fragment").collect();
    assert_eq!(items.len(), 2, "{:?}", symbol.rust_contract);
    let snippets: Vec<_> = items.iter().map(|item| &source[item.span.start_byte as usize..item.span.end_byte as usize]).collect();
    assert!(snippets.contains(&"$lt:lifetime"));
    assert!(snippets.contains(&"$item:lifetime"));
    assert!(items.iter().all(|item| item.text.contains("not a runtime variable")
        && item.text.contains("repetition") && item.text.contains("does not create a borrow")));
}
