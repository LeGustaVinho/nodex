use nodex_ir::SymbolKind;
use nodex_lang_rust::lower_source;

#[test]
fn recovered_simple_trait_alias_is_explained_on_its_enclosing_module() {
    let source = "#![feature(trait_alias)]\ntrait Alias = A + B; mod inner { trait Local = C + D; } mod plain { trait Normal {} } fn text() { let s = \"trait Fake = A + B;\"; }";
    let file = lower_source("src/lib.rs", source);
    let root = file.symbols.iter().find(|symbol| symbol.kind == SymbolKind::Module && symbol.parent.is_none()).unwrap();
    let inner = file.symbols.iter().find(|symbol| symbol.name == "inner").unwrap();
    for (symbol, expected) in [(root, "trait Alias = A + B;"), (inner, "trait Local = C + D;")] {
        let items: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Unstable trait alias").collect();
        assert_eq!(items.len(), 1, "{:?}", symbol.rust_contract);
        assert_eq!(&source[items[0].span.start_byte as usize..items[0].span.end_byte as usize], expected);
        assert!(items[0].text.contains("does not declare a new trait"));
        assert!(items[0].text.contains("unstable"));
        assert!(items[0].text.contains("nightly"));
        assert!(items[0].text.contains("do not establish"));
    }
    let plain = file.symbols.iter().find(|symbol| symbol.name == "plain").unwrap();
    assert!(!plain.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Unstable trait alias"));
    assert!(!file.diagnostics.is_empty(), "the parser diagnostic must remain visible");
}
