use nodex_lang_rust::lower_source;
#[test]
fn type_fragments_are_explained_only_in_matchers_including_repetitions() {
    let source = "macro_rules! m { ($t:ty) => { stringify!($t:ty) }; ($( $kind:ty ),*) => { $($kind)* }; ($(($inner:ty)),*) => { () }; ($name:ident) => { stringify!($name) }; }";
    let ir = lower_source("src/lib.rs", source);
    let symbol = ir.symbols.iter().find(|symbol| symbol.name == "m!").unwrap();
    let items: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Macro type fragment").collect();
    assert_eq!(items.len(), 3, "{:?}", symbol.rust_contract);
    let spans: Vec<_> = items.iter().map(|item| &source[item.span.start_byte as usize..item.span.end_byte as usize]).collect();
    assert_eq!(spans, ["$t:ty", "$kind:ty", "$inner:ty"]);
    assert!(!items[0].text.contains("inside a repetition"));
    assert!(items[1..].iter().all(|item| item.text.contains("inside a repetition")));
    assert!(items.iter().all(|item| item.text.contains("parses as a Rust type") && item.text.contains("not a runtime value") && item.text.contains("still resolves and checks")));
}
