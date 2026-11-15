use nodex_ir::SymbolKind;
use nodex_lang_rust::lower_source;

#[test]
fn repeated_unnamed_unit_constants_are_explained_on_the_enclosing_scope() {
    let source = "const _: () = (); const _: () = (); const NAMED: () = (); fn f() { const _: () = (); }";
    let file = lower_source("src/lib.rs", source);
    let root = file.symbols.iter().find(|symbol| symbol.parent.is_none() && symbol.kind == SymbolKind::Module).unwrap();
    let items: Vec<_> = root.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Unnamed unit constant").collect();
    assert_eq!(items.len(), 2, "{:?}", root.rust_contract);
    for item in items {
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "const _: () = ();");
        assert!(item.text.contains("no usable name") && item.text.contains("Multiple unnamed constants") && item.text.contains("constant expression") && item.text.contains("sole value"));
    }
    let function = file.symbols.iter().find(|symbol| symbol.name == "f").unwrap();
    assert!(function.rust_contract.as_ref().unwrap().iter().any(|item| item.group == "Unnamed unit constant"));
}
