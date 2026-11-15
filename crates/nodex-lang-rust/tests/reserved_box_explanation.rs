use nodex_ir::SymbolKind;
use nodex_lang_rust::lower_source;

#[test]
fn reserved_box_is_distinct_from_type_raw_identifier_and_token_data() {
    let source = r##"fn box() {} fn f() { let box = 1; let r#box = 2; let _ = "box"; /* box */ custom!(box); let _: Box<i32> = Box::new(1); } mod inner { fn box() {} } mod clean { fn f() { let r#box = "box"; custom!(box); } }"##;
    let ir = lower_source("src/lib.rs", source);
    let root = ir.symbols.iter().find(|symbol| symbol.parent.is_none() && symbol.kind == SymbolKind::Module).unwrap();
    let items: Vec<_> = root.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Reserved word box").collect();
    assert_eq!(items.len(), 2, "{items:?}");
    for item in items {
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "box");
        assert!(item.text.contains("reserved Rust keyword") && item.text.contains("r#box") && item.text.contains("Box::new") && item.text.contains("if `Box` resolves") && item.text.contains("stable Rust"));
    }
    let inner = ir.symbols.iter().find(|symbol| symbol.name == "inner").unwrap();
    assert_eq!(inner.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Reserved word box").count(), 1);
    let clean = ir.symbols.iter().find(|symbol| symbol.name == "clean").unwrap();
    assert!(!clean.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Reserved word box"));
}
