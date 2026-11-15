use nodex_lang_rust::lower_source;

#[test]
fn inner_line_docs_explain_the_enclosing_module_not_the_next_item() {
    let source = "//! Crate docs\nmod inner {\n //! Inline docs\n /// Outer docs\n fn method() {}\n}\n";
    let ir = lower_source("src/lib.rs", source);
    let root = ir.symbols.iter().find(|s| s.kind == nodex_ir::SymbolKind::Module && s.parent.is_none()).unwrap();
    let inner = ir.symbols.iter().find(|s| s.kind == nodex_ir::SymbolKind::Module && s.name == "inner").unwrap();
    for symbol in [root, inner] {
        let facts: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Inner documentation comment").collect();
        assert_eq!(facts.len(), 1, "{}: {:?}", symbol.name, symbol.rust_contract);
        let item = facts[0];
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "//!");
        assert!(item.text.contains("enclosing module"));
        assert!(item.text.contains("rather than the item that follows"));
        assert!(item.text.contains("not an instruction executed"));
    }
    assert!(root.doc.as_deref().unwrap_or_default().contains("Crate docs"));
    let method = ir.symbols.iter().find(|s| s.name == "method").unwrap();
    assert!(!method.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Inner documentation comment"));
    assert!(method.doc.as_deref().unwrap_or_default().contains("Outer docs"));
}
