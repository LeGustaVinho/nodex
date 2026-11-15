use nodex_lang_rust::lower_source;

#[test]
fn path_matcher_fragments_include_repetitions_but_exclude_transcriber_and_other_fragments() {
    let source = "macro_rules! m { ($x:path, $( $item:path ),*) => { $x }; ($i:ident) => { $i:path }; ($e:expr) => { $e }; }";
    let ir = lower_source("src/lib.rs", source);
    let m = ir.symbols.iter().find(|symbol| symbol.name == "m!").expect("macro symbol");
    let items: Vec<_> = m.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Macro path fragment").collect();
    assert_eq!(items.len(), 2, "{:?}", m.rust_contract);
    let spans: Vec<_> = items.iter().map(|item| &source[item.span.start_byte as usize..item.span.end_byte as usize]).collect();
    assert_eq!(spans, ["$x:path", "$item:path"]);
    for item in items {
        assert!(item.text.contains("type-path syntax"));
        assert!(item.text.contains("neither resolves the path"));
        assert!(item.text.contains("each matched iteration"));
    }
}
