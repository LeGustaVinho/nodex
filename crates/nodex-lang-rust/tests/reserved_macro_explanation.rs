use nodex_ir::SymbolKind;
use nodex_lang_rust::lower_source;

#[test]
fn macro_keyword_is_explained_without_conflating_macro_rules_or_raw_names() {
    let source = r##"fn macro() {} fn r#macro() {} fn valid() { let _ = "macro"; let _ = r#"macro"#; /* macro */ } // macro
macro_rules! tokens { (macro) => {} }"##;
    let file = lower_source("src/lib.rs", source);
    let root = file.symbols.iter().find(|s| s.kind == SymbolKind::Module && s.parent.is_none()).unwrap();
    let matches: Vec<_> = root.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Reserved word macro").collect();
    assert_eq!(matches.len(), 1, "{:?}", root.rust_contract);
    assert_eq!(&source[matches[0].span.start_byte as usize..matches[0].span.end_byte as usize], "macro");
    for word in ["reserved", "r#macro", "decl_macro", "nightly", "macro_rules!"] { assert!(matches[0].text.contains(word), "{}", matches[0].text); }
}

#[test]
fn experimental_macro_definition_shows_context() {
    let source = "macro example() {}";
    let file = lower_source("src/lib.rs", source);
    let root = file.symbols.iter().find(|s| s.kind == SymbolKind::Module && s.parent.is_none()).unwrap();
    assert!(root.rust_contract.as_ref().unwrap().iter().any(|item| item.group == "Reserved word macro"), "{:?}", root.rust_contract);
}
