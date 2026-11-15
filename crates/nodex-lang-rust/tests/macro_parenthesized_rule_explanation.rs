use nodex_lang_rust::lower_source;

#[test]
fn explains_expression_matcher_and_parenthesized_transcriber() {
    let source = "macro_rules! m { ($x:expr) => ($x + 1); ($x:expr) => { $x }; ($x:expr) => [$x]; }";
    let ir = lower_source("src/lib.rs", source);
    let symbol = ir.symbols.iter().find(|symbol| symbol.name == "m!").unwrap();
    let items: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Macro rule with parenthesized expansion").collect();
    assert_eq!(items.len(), 1, "{:?}", symbol.rust_contract);
    let item = items[0];
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "($x:expr) => ($x + 1)");
    assert!(item.text.contains("accepts an expression fragment") && item.text.contains("not automatically emitted") && item.text.contains("does not itself evaluate") && item.text.contains("first matching rule"));
}
