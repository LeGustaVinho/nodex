use nodex_ir::SymbolKind;
use nodex_lang_rust::lower_source;

#[test]
fn reserved_unsized_is_visible_without_confusing_raw_names_text_and_macros() {
    let source = r##"
        unsized fn invalid_modifier() {}
        fn unsized() {}
        fn example(unsized: i32) { let unsized = 1; let r#unsized = 2; let _ = "unsized"; /* unsized */ custom!(unsized); }
        mod inner { fn f() { let unsized = 0; } }
        mod clean { fn f() { let r#unsized = "unsized"; /* unsized */ } }
    "##;
    let ir = lower_source("src/lib.rs", source);
    let root = ir.symbols.iter().find(|symbol| symbol.parent.is_none() && symbol.kind == SymbolKind::Module).unwrap();
    let items: Vec<_> = root.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Reserved unsized word").collect();
    assert_eq!(items.len(), 4, "{items:?}");
    for item in items {
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "unsized");
        for phrase in ["reserved Rust keyword", "no language behavior", "`r#unsized`", "distinct from dynamically sized types", "slices or `str`", "syntax is invalid", "macro token input"] {
            assert!(item.text.contains(phrase), "{}", item.text);
        }
    }
    let inner = ir.symbols.iter().find(|symbol| symbol.name == "inner").unwrap();
    assert_eq!(inner.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Reserved unsized word").count(), 1);
    let clean = ir.symbols.iter().find(|symbol| symbol.name == "clean").unwrap();
    assert!(!clean.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Reserved unsized word"));
}
