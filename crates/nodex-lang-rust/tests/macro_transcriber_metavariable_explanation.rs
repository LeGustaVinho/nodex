use nodex_lang_rust::lower_source;

#[test]
fn explains_captured_names_in_transcribers_including_repetitions_only() {
    let source = "macro_rules! demo { ($x:expr) => { $x + 1 }; ($( $i:ident ),*) => { $( $i ),* }; ($y:expr) => { \"$y\" }; }";
    let ir = lower_source("src/lib.rs", source);
    let symbol = ir.symbols.iter().find(|symbol| symbol.name == "demo!").unwrap();
    let items: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Macro expansion metavariable").collect();
    assert_eq!(items.len(), 2, "{:?}", symbol.rust_contract);
    let snippets: Vec<_> = items.iter().map(|item| &source[item.span.start_byte as usize..item.span.end_byte as usize]).collect();
    assert!(snippets.contains(&"$x") && snippets.contains(&"$i"));
    assert!(items.iter().all(|item| item.text.contains("matcher") && item.text.contains("not a runtime variable")
        && item.text.contains("does not evaluate") && item.text.contains("repetition")));
}
