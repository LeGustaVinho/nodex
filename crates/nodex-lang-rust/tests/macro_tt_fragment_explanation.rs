use nodex_lang_rust::lower_source;

#[test]
fn token_tree_fragment_in_matcher_including_repetition_not_transcriber() {
    let source = "macro_rules! tokens { ($x:tt) => { stringify!($x:tt) }; ($( $part:tt ),*) => { $( use_token!($part); )* }; ($e:expr) => { $e }; } fn unrelated() {}";
    let ir = lower_source("src/lib.rs", source);
    let symbol = ir.symbols.iter().find(|s| s.name == "tokens!").unwrap();
    let items: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Macro token tree fragment").collect();
    assert_eq!(items.len(), 2, "{:?}", symbol.rust_contract);
    let spans: Vec<_> = items.iter().map(|item| &source[item.span.start_byte as usize..item.span.end_byte as usize]).collect();
    assert_eq!(spans, ["$x:tt", "$part:tt"]);
    assert!(items.iter().all(|item| item.text.contains("single token") && item.text.contains("matched delimiter pair") && item.text.contains("ungrouped") && item.text.contains("does not evaluate")));
    let ordinary = ir.symbols.iter().find(|s| s.name == "unrelated").unwrap();
    assert!(!ordinary.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Macro token tree fragment"));
}
