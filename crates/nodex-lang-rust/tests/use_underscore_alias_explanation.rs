use nodex_ir::SymbolKind;
use nodex_lang_rust::lower_source;

#[test]
fn underscore_alias_has_anonymous_import_explanation_in_its_module() {
    let source = "use a::b as _; use a::c as local; mod inner { use x::Trait as _; }";
    let ir = lower_source("src/lib.rs", source);
    let root = ir.symbols.iter().find(|s| s.parent.is_none() && s.kind == SymbolKind::Module).unwrap();
    let items: Vec<_> = root.rust_contract.as_ref().unwrap().iter().filter(|i| i.group == "Anonymous import").collect();
    assert_eq!(items.len(), 1);
    assert_eq!(&source[items[0].span.start_byte as usize..items[0].span.end_byte as usize], "use a::b as _;");
    assert!(items[0].text.contains("a::b") && items[0].text.contains("no usable local name") && items[0].text.contains("If the path resolves to a trait") && items[0].text.contains("does not establish"));
    let inner = ir.symbols.iter().find(|s| s.name == "inner").unwrap();
    assert_eq!(inner.rust_contract.as_ref().unwrap().iter().filter(|i| i.group == "Anonymous import").count(), 1);
}
