use nodex_lang_rust::lower_source;

#[test]
fn statement_fragments_are_explained_only_in_matchers_including_repetition() {
    let source = "macro_rules! m { ($x:stmt) => { $x }; ($( $s:stmt );*) => { stringify!($s:stmt) }; ($e:expr) => { $e }; } macro_rules! plain { ($e:expr) => { stringify!($fake:stmt) }; }";
    let ir = lower_source("src/lib.rs", source);
    let mac = ir.symbols.iter().find(|symbol| symbol.name == "m!").unwrap();
    let facts: Vec<_> = mac.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Macro statement fragment").collect();
    assert_eq!(facts.len(), 2, "{:?}", mac.rust_contract);
    assert_eq!(&source[facts[0].span.start_byte as usize..facts[0].span.end_byte as usize], "$x:stmt");
    assert_eq!(&source[facts[1].span.start_byte as usize..facts[1].span.end_byte as usize], "$s:stmt");
    for fact in &facts {
        assert!(fact.text.contains("normally excludes a trailing semicolon") && fact.text.contains("item statement") && fact.text.contains("must match separately"));
        assert!(fact.text.contains("rather than executing") && fact.text.contains("transcriber"));
    }
    assert!(facts[1].text.contains("Inside this repetition"));
    let plain = ir.symbols.iter().find(|symbol| symbol.name == "plain!").unwrap();
    assert!(!plain.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Macro statement fragment"));
}
