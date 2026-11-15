use nodex_lang_rust::lower_source;

#[test]
fn matcher_repetition_explains_semicolon_and_zero_or_more_not_transcriber() {
    let source = "macro_rules! m { ($($x:expr);*) => { $($x);* }; ($( $($v:expr),* );*) => { () }; ($($z:expr),*) => { () }; } macro_rules! other { ($x:expr) => { $($x);* }; }";
    let file = lower_source("src/lib.rs", source);
    let m = file.symbols.iter().find(|s| s.name == "m!").unwrap();
    let items: Vec<_> = m.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Semicolon-separated macro matcher repetition").collect();
    assert_eq!(items.len(), 2, "{:?}", m.rust_contract);
    for item in items {
        let span = &source[item.span.start_byte as usize..item.span.end_byte as usize];
        assert!(span.ends_with(";*"), "{span}");
        assert!(item.text.contains("zero or more"));
        assert!(item.text.contains("does not allow a trailing semicolon"));
        assert!(item.text.contains("not a runtime loop"));
    }
    let other = file.symbols.iter().find(|s| s.name == "other!").unwrap();
    assert!(!other.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Semicolon-separated macro matcher repetition"));
}
