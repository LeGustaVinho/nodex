use nodex_lang_rust::lower_source;

#[test]
fn explains_pat_param_in_matchers_including_repetition_but_not_transcribers() {
    let source = "macro_rules! m { ($p:pat_param) => { $p }; ($($part:pat_param)|+) => { () }; } macro_rules! other { ($x:expr) => { $x:pat_param }; }";
    let file = lower_source("src/lib.rs", source);
    let symbol = file.symbols.iter().find(|symbol| symbol.name == "m!").unwrap();
    let items: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Macro pattern fragment without top-level alternatives").collect();
    assert_eq!(items.len(), 2, "{:?}", symbol.rust_contract);
    assert!(items.iter().any(|item| item.text.contains("$p:pat_param")));
    assert!(items.iter().any(|item| item.text.contains("$part:pat_param")));
    for item in items {
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "pat_param");
        assert!(item.text.contains("without a top-level | alternative"));
        assert!(item.text.contains("nested inside another pattern"));
        assert!(item.text.contains("edition"));
    }
    let other = file.symbols.iter().find(|symbol| symbol.name == "other!").unwrap();
    assert!(!other.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Macro pattern fragment without top-level alternatives"));
}
