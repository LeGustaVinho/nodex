use nodex_lang_rust::lower_source;

#[test]
fn block_fragment_is_explained_in_matcher_including_repetition_not_transcriber() {
    let source = "macro_rules! blocks { ($x:block) => { stringify!($x:block) }; ($( $part:block ),*) => { $( use_block($part); )* }; ($e:expr) => { $e }; } fn unrelated() {}";
    let ir = lower_source("src/lib.rs", source);
    let symbol = ir.symbols.iter().find(|s| s.name == "blocks!").unwrap();
    let items: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Macro block fragment").collect();
    assert_eq!(items.len(), 2, "{:?}", symbol.rust_contract);
    let spans: Vec<_> = items.iter().map(|item| &source[item.span.start_byte as usize..item.span.end_byte as usize]).collect();
    assert_eq!(spans, ["$x:block", "$part:block"]);
    assert!(items.iter().all(|item| item.text.contains("block expression") && item.text.contains("not an ordinary runtime variable") && item.text.contains("does not execute the block") && item.text.contains("repetition")));
    let function = ir.symbols.iter().find(|s| s.name == "unrelated").unwrap();
    assert!(!function.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Macro block fragment"));
}
