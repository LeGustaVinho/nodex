use nodex_lang_rust::lower_source;

#[test]
fn expr_matcher_fragments_include_repetitions_but_not_transcribers_or_other_fragments() {
    let source = "macro_rules! m { ($x:expr, $( $item:expr ),*) => { $x }; ($i:ident) => { $i:expr }; ($old:expr_2021) => { $old }; }";
    let ir = lower_source("src/lib.rs", source);
    let m = ir.symbols.iter().find(|symbol| symbol.name == "m!").expect("macro symbol");
    let items: Vec<_> = m.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Macro expression fragment").collect();
    assert_eq!(items.len(), 2, "{:?}", m.rust_contract);
    let spans: Vec<_> = items.iter().map(|item| &source[item.span.start_byte as usize..item.span.end_byte as usize]).collect();
    assert_eq!(spans, ["$x:expr", "$item:expr"]);
    for item in items {
        assert!(item.text.contains("not an ordinary runtime variable"));
        assert!(item.text.contains("does not evaluate"));
        assert!(item.text.contains("edition of the macro definition"));
        assert!(item.text.contains("each matched iteration"));
    }
}
