use nodex_ir::SymbolKind;
use nodex_lang_rust::lower_source;

#[test]
fn reserved_priv_is_visible_without_confusing_identifiers_and_token_data() {
    let source = r##"
        priv fn invalid_modifier() {}
        fn priv() {}
        fn example(priv: i32) { let priv = 1; let r#priv = 2; let _ = "priv"; /* priv */ custom!(priv); }
        mod inner { fn f() { let priv = 0; } }
        mod clean { #[allow(priv)] fn f<'priv>() { let r#priv = "priv"; /* priv */ custom!(priv); } }
    "##;
    let ir = lower_source("src/lib.rs", source);
    let root = ir.symbols.iter().find(|symbol| symbol.parent.is_none() && symbol.kind == SymbolKind::Module).unwrap();
    let items: Vec<_> = root.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Reserved keyword priv").collect();
    assert_eq!(items.len(), 4, "{items:?}");
    for item in items {
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "priv");
        assert!(item.text.contains("reserved Rust keyword") && item.text.contains("not a visibility modifier"));
        assert!(item.text.contains("module and descendants") && item.text.contains("trait items") && item.text.contains("`r#priv`") && item.text.contains("does not validate"));
    }
    let inner = ir.symbols.iter().find(|symbol| symbol.name == "inner").unwrap();
    assert_eq!(inner.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Reserved keyword priv").count(), 1);
    let clean = ir.symbols.iter().find(|symbol| symbol.name == "clean").unwrap();
    assert!(!clean.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Reserved keyword priv"));
}
