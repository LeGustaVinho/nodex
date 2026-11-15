use nodex_ir::SymbolKind;
use nodex_lang_rust::lower_source;

#[test]
fn reserved_override_is_visible_but_raw_identifiers_and_text_are_not() {
    let source = r##"fn override() {} fn r#override() {} fn valid() { let _ = "override"; let _ = r#"override"#; /* override */ } // override
macro_rules! tokens { (override) => {} }
mod inner { fn override() {} }"##;
    let file = lower_source("src/lib.rs", source);
    let root = file.symbols.iter().find(|s| s.kind == SymbolKind::Module && s.parent.is_none()).unwrap();
    let matches: Vec<_> = root.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Reserved word override").collect();
    assert_eq!(matches.len(), 1, "{:?}", root.rust_contract);
    assert_eq!(&source[matches[0].span.start_byte as usize..matches[0].span.end_byte as usize], "override");
    assert!(matches[0].text.contains("reserved"));
    assert!(matches[0].text.contains("r#override"));
    assert!(matches[0].text.contains("does not declare or perform method overriding"));
    let inner = file.symbols.iter().find(|s| s.kind == SymbolKind::Module && s.name == "inner").unwrap();
    assert_eq!(inner.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Reserved word override").count(), 1);
}
