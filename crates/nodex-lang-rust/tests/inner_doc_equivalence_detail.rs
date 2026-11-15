use nodex_ir::SymbolKind;
use nodex_lang_rust::lower_source;

#[test]
fn inner_doc_line_and_attribute_show_matching_text_in_correct_module() {
    let source = "//! text\n#![doc = \" text\"]\nmod inner { //! nested\n #![doc = r#\" nested\"#] fn f() {} }\n/// outer\nfn g() {}";
    let ir = lower_source("src/lib.rs", source);
    let root = ir.symbols.iter().find(|symbol| symbol.parent.is_none() && symbol.kind == SymbolKind::Module).unwrap();
    let items: Vec<_> = root.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Inner documentation equivalence").collect();
    assert_eq!(items.len(), 2, "{items:?}");
    assert!(items.iter().any(|item| item.text.contains("text \" text\"") && item.text.contains("equivalent")));
    assert!(items.iter().any(|item| item.text.contains("written string is \" text\"") && item.text.contains("including the leading space")));
    for item in items { assert!(item.text.contains("not the following item") && item.text.contains("runtime")); }
    let inner = ir.symbols.iter().find(|symbol| symbol.name == "inner").unwrap();
    let inner_items: Vec<_> = inner.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Inner documentation equivalence").collect();
    assert_eq!(inner_items.len(), 2);
    assert!(inner_items.iter().any(|item| item.text.contains("r#\" nested\"#")));
    assert!(inner_items.iter().any(|item| item.text.contains("\" nested\"")));
}
