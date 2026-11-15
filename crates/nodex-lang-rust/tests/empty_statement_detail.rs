use nodex_lang_rust::lower_source;

#[test]
fn standalone_semicolons_are_explained_on_their_function_symbol() {
    let source = "fn f() { ; let x = 1; ; let text = \";\"; /* ; */ } fn g() { ; } fn empty() {}";
    let file = lower_source("src/lib.rs", source);
    for (name, count) in [("f", 2), ("g", 1)] {
        let symbol = file.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let items: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Empty statement").collect();
        assert_eq!(items.len(), count, "{name}: {:?}", symbol.rust_contract);
        for item in items {
            assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], ";");
            assert!(item.text.contains("performs no computation"));
            assert!(item.text.contains("differs from the semicolon"));
        }
    }
    let empty = file.symbols.iter().find(|symbol| symbol.name == "empty").unwrap();
    assert!(!empty.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Empty statement"));
}
