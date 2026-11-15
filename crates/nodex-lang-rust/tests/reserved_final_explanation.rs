use nodex_ir::SymbolKind;
use nodex_lang_rust::lower_source;

#[test]
fn reserved_final_is_visible_even_in_invalid_syntax_without_raw_or_text_false_positives() {
    let source = r##"
        final fn invalid_modifier() {}
        fn final() {}
        fn example(final: i32) { let final = 1; let r#final = 2; let _ = "final"; /* final */ custom!(final); }
        mod inner { fn f() { let final = 0; } }
        mod clean { fn f() { let r#final = "final"; /* final */ } }
    "##;
    let ir = lower_source("src/lib.rs", source);
    let root = ir.symbols.iter().find(|symbol| symbol.parent.is_none() && symbol.kind == SymbolKind::Module).unwrap();
    let items: Vec<_> = root.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Reserved Rust word").collect();
    assert_eq!(items.len(), 4, "{items:?}");
    for item in items {
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "final");
        for phrase in ["reserved Rust keyword", "no current language behavior", "C#-style modifier", "`r#final`", "surrounding Rust syntax is invalid", "macro token input"] {
            assert!(item.text.contains(phrase), "{}", item.text);
        }
    }
    let inner = ir.symbols.iter().find(|symbol| symbol.name == "inner").unwrap();
    assert_eq!(inner.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Reserved Rust word").count(), 1);
    let clean = ir.symbols.iter().find(|symbol| symbol.name == "clean").unwrap();
    assert!(!clean.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Reserved Rust word"));
}
