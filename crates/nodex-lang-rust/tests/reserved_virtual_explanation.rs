use nodex_ir::SymbolKind;
use nodex_lang_rust::lower_source;

#[test]
fn reserved_virtual_is_visible_even_in_recovered_invalid_syntax() {
    let source = r##"
        virtual fn invalid_modifier() {}
        fn virtual() {}
        fn example(virtual: i32) { let virtual = 1; let r#virtual = 2; let _ = "virtual"; /* virtual */ custom!(virtual); }
        mod inner { fn f() { let virtual = 0; } }
        mod clean { #[allow(virtual)] fn f<'virtual>() { let r#virtual = "virtual"; /* virtual */ custom!(virtual); } }
    "##;
    let ir = lower_source("src/lib.rs", source);
    let root = ir.symbols.iter().find(|symbol| symbol.parent.is_none() && symbol.kind == SymbolKind::Module).unwrap();
    let items: Vec<_> = root.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Reserved keyword virtual").collect();
    assert_eq!(items.len(), 4, "{items:?}");
    for item in items {
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "virtual");
        assert!(item.text.contains("reserved Rust keyword") && item.text.contains("does not enable virtual dispatch")
            && item.text.contains("`r#virtual`") && item.text.contains("does not validate"));
    }
    let inner = ir.symbols.iter().find(|symbol| symbol.name == "inner").unwrap();
    assert_eq!(inner.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Reserved keyword virtual").count(), 1);
    let clean = ir.symbols.iter().find(|symbol| symbol.name == "clean").unwrap();
    assert!(!clean.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Reserved keyword virtual"));
}
