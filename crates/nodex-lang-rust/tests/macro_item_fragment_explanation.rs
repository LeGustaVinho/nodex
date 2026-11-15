use nodex_lang_rust::lower_source;

#[test]
fn explains_item_fragments_in_matchers_including_repetition_but_not_transcribers() {
    let source = "macro_rules! m { ($x:item) => { $x }; ($($it:item)*) => { $($it)* }; } macro_rules! no { ($x:tt) => { $x:item }; }";
    let file = lower_source("src/lib.rs", source);
    let symbol = file.symbols.iter().find(|symbol| symbol.name == "m!").unwrap();
    let items: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Macro item fragment").collect();
    assert_eq!(items.len(), 2, "{:?}", symbol.rust_contract);
    assert!(items.iter().any(|item| &source[item.span.start_byte as usize..item.span.end_byte as usize] == "$x:item"));
    assert!(items.iter().any(|item| &source[item.span.start_byte as usize..item.span.end_byte as usize] == "$it:item"));
    assert!(items.iter().all(|item| item.text.contains("one complete Rust item") && item.text.contains("does not execute") && item.text.contains("repetition")));
    let other = file.symbols.iter().find(|symbol| symbol.name == "no!").unwrap();
    assert!(!other.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Macro item fragment"));
}
