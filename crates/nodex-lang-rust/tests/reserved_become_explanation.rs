use nodex_lang_rust::lower_source;
#[test]
fn become_reserved_and_experimental_tail_calls_are_visible_without_text_false_positives() {
    let source = r##"fn f() -> i32 { become next() } fn become() {} fn r#become() { let text = "become"; let raw = r#"become"#; /* become */ m!(become); } mod inner { fn g() { become next(); } }"##;
    let ir = lower_source("src/lib.rs", source);
    let root = ir.symbols.iter().find(|s| s.kind == nodex_ir::SymbolKind::Module && s.parent.is_none()).unwrap();
    let inner = ir.symbols.iter().find(|s| s.kind == nodex_ir::SymbolKind::Module && s.name == "inner").unwrap();
    for (symbol, expected) in [(root, 2), (inner, 1)] {
        let items: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Reserved keyword become").collect();
        assert_eq!(items.len(), expected, "{}: {:?}", symbol.name, symbol.rust_contract);
        for item in items {
            assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "become");
            assert!(item.text.contains("reserved Rust keyword") && item.text.contains("r#become") && item.text.contains("experimental, incomplete") && item.text.contains("nightly compiler") && item.text.contains("drop order") && item.text.contains("does not establish"));
        }
    }
}
