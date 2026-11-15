use nodex_lang_rust::lower_source;

#[test]
fn explains_pat_in_matchers_including_repetition_but_not_transcribers() {
    let source = "macro_rules! m { ($x:pat) => { $x }; ($($p:pat),*) => { ($($p),*) }; ($v:pat_param) => { $z:pat }; ($i:ident) => { $i }; }";
    let ir = lower_source("src/lib.rs", source);
    let symbol = ir.symbols.iter().find(|symbol| symbol.name == "m!").unwrap();
    let facts: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Macro pattern fragment").collect();
    assert_eq!(facts.len(), 2, "{:?}", symbol.rust_contract);
    let spans: Vec<_> = facts.iter().map(|item| &source[item.span.start_byte as usize..item.span.end_byte as usize]).collect();
    assert_eq!(spans, ["$x:pat", "$p:pat"]);
    assert!(facts.iter().all(|item| item.text.contains("edition of the macro definition") && item.text.contains("2021") && item.text.contains("top-level `|`") && item.text.contains("pat_param") && item.text.contains("does not evaluate")));
}
