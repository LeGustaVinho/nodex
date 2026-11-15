use nodex_lang_rust::lower_source;
#[test]
fn reserved_abstract_is_visible_even_when_invalid_and_not_confused_with_raw_or_text() {
    let source = r##"abstract class Foo {} fn abstract() {} fn r#abstract() { let text = "abstract"; let raw = r#"abstract"#; /* abstract */ m!(abstract); } mod nested { fn abstract() {} }"##;
    let ir = lower_source("src/lib.rs", source);
    let root = ir.symbols.iter().find(|s| s.kind == nodex_ir::SymbolKind::Module && s.parent.is_none()).unwrap();
    let nested = ir.symbols.iter().find(|s| s.kind == nodex_ir::SymbolKind::Module && s.name == "nested").unwrap();
    for (symbol, expected) in [(root, 2), (nested, 1)] {
        let items: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Reserved keyword abstract").collect();
        assert_eq!(items.len(), expected, "{}: {:?}", symbol.name, symbol.rust_contract);
        for item in items {
            assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "abstract");
            assert!(item.text.contains("reserved Rust keyword") && item.text.contains("invalid Rust syntax") && item.text.contains("r#abstract") && item.text.contains("not a direct equivalent"));
        }
    }
}
