use nodex_ir::SymbolKind;
use nodex_lang_rust::lower_source;

#[test]
fn current_crate_alias_is_explained_in_its_own_module() {
    let source = "extern crate self as alias; mod inner { extern crate self as local; } mod other { extern crate foo as alias; }";
    let file = lower_source("src/lib.rs", source);
    assert!(!file.imports.iter().any(|import| import.target == "pkg:self"), "{:?}", file.imports);
    assert!(file.imports.iter().any(|import| import.target == "pkg:foo"), "{:?}", file.imports);
    let root = file.symbols.iter().find(|symbol| symbol.kind == SymbolKind::Module && symbol.parent.is_none()).unwrap();
    let inner = file.symbols.iter().find(|symbol| symbol.name == "inner").unwrap();
    for (symbol, alias, expected) in [(root, "alias", "extern crate self as alias;"), (inner, "local", "extern crate self as local;")] {
        let items: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Alias for the current crate").collect();
        assert_eq!(items.len(), 1, "{symbol:?}");
        assert_eq!(&source[items[0].span.start_byte as usize..items[0].span.end_byte as usize], expected);
        assert!(items[0].text.contains(&format!("binds the name {alias}")));
        assert!(items[0].text.contains("not an external dependency"));
        assert!(items[0].text.contains("normal scope and visibility"));
    }
    let other = file.symbols.iter().find(|symbol| symbol.name == "other").unwrap();
    assert!(!other.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Alias for the current crate"));
}
