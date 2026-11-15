use nodex_lang_rust::lower_source;

#[test]
fn inner_attributes_are_explained_on_their_own_scope_not_the_next_item() {
    let source = "#![allow(unused)] mod inner { #![allow(dead_code)] fn f() { #![allow(unused_variables)] let x = 1; } fn plain() {} }";
    let ir = lower_source("src/lib.rs", source);
    let root = ir.symbols.iter().find(|s| s.kind == nodex_ir::SymbolKind::Module && s.parent.is_none()).unwrap();
    let module = ir.symbols.iter().find(|s| s.kind == nodex_ir::SymbolKind::Module && s.name == "inner").unwrap();
    let function = ir.symbols.iter().find(|s| s.name == "f").unwrap();
    for (symbol, expected) in [(root, "#![allow(unused)]"), (module, "#![allow(dead_code)]"), (function, "#![allow(unused_variables)]")] {
        let facts: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Inner attribute").collect();
        assert_eq!(facts.len(), 1, "{}: {:?}", symbol.name, symbol.rust_contract);
        assert_eq!(&source[facts[0].span.start_byte as usize..facts[0].span.end_byte as usize], expected);
        assert!(facts[0].text.contains("enclosing scope, not to the next item") && facts[0].text.contains("meaning depends") && facts[0].text.contains("not an instruction executed"));
    }
    let plain = ir.symbols.iter().find(|s| s.name == "plain").unwrap();
    assert!(!plain.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Inner attribute"));
}

#[test]
fn inner_block_attribute_is_attached_to_the_enclosing_flow() {
    let source = "fn f() { let result = { #![allow(unused_variables)] let x = 1; 7 }; }";
    let ir = lower_source("src/lib.rs", source);
    let flow = ir.flows.iter().find(|flow| flow.name == "f").unwrap();
    let facts: Vec<_> = flow.reading.iter().flatten().filter(|site| matches!(&site.content,
        nodex_ir::reading::ReadingContent::Detail { label, .. } if label == "Attribute on the enclosing block")).collect();
    assert_eq!(facts.len(), 1, "{:?}", flow.reading);
    assert_eq!(&source[facts[0].span.start_byte as usize..facts[0].span.end_byte as usize], "#![allow(unused_variables)]");
}
