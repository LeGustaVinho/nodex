use nodex_ir::SymbolKind;
use nodex_lang_rust::lower_source;

#[test]
fn ordinary_block_comments_explain_nesting_on_owning_symbols() {
    let source = "/* simple */ fn f() { /* outer /* inner */ tail */ let x = \"/* not a comment */\"; } /** documentation */ fn documented() {} mod inner { /* child */ }";
    let ir = lower_source("src/lib.rs", source);
    let root = ir.symbols.iter().find(|symbol| symbol.parent.is_none() && symbol.kind == SymbolKind::Module).unwrap();
    let items: Vec<_> = root.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Block comment").collect();
    assert_eq!(items.len(), 1, "{items:?}");
    assert_eq!(items[0].span.start_byte as usize, source.find("/* simple").unwrap());
    assert!(items[0].text.contains("not executed") && items[0].text.contains("inside") && items[0].text.contains("outer comment continues") && items[0].text.contains("Documentation comments"));
    let f = ir.symbols.iter().find(|symbol| symbol.name == "f").unwrap();
    let item = f.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Block comment").unwrap();
    assert_eq!(item.span.start_byte as usize, source.find("/* outer").unwrap());
    let inner = ir.symbols.iter().find(|symbol| symbol.name == "inner").unwrap();
    assert_eq!(inner.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Block comment").count(), 1);
}
