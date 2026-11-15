use nodex_ir::SymbolKind;
use nodex_lang_rust::lower_source;

#[test]
fn reserved_raw_identifiers_are_explained_in_recovered_source() {
    let source = r###"fn f() { let r#self = 1; let r#Self = 1; let r#super = 1; let r#crate = 1; let r#_ = 1; let r#type = 1; let text = "r#self"; custom!(r#self); }"###;
    let ir = lower_source("src/lib.rs", source);
    let root = ir.symbols.iter().find(|s| s.parent.is_none() && s.kind == SymbolKind::Module).unwrap();
    let items: Vec<_> = root.rust_contract.as_ref().unwrap().iter().filter(|i| i.group == "Reserved raw identifier").collect();
    let mut actual: Vec<_> = items.iter().map(|i| &source[i.span.start_byte as usize..i.span.end_byte as usize]).collect();
    actual.sort();
    assert_eq!(actual, ["r#Self", "r#_", "r#crate", "r#self", "r#super"]);
    assert!(items.iter().all(|i| i.text.contains("cannot be used as a raw identifier") && i.text.contains("does not make the program valid")));
}
