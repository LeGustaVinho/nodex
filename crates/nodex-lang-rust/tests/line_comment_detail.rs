use nodex_ir::SymbolKind;
use nodex_lang_rust::lower_source;

#[test]
fn explains_ordinary_line_comments_without_confusing_docs_or_strings() {
    let source = "// root\nfn f() { // ordinary\n let s = \"// string\"; //// ordinary too\n }\n/// documentation\nfn documented() {}";
    let file = lower_source("src/lib.rs", source);
    let root = file.symbols.iter().find(|symbol| symbol.parent.is_none() && symbol.kind == SymbolKind::Module).unwrap();
    let root_items: Vec<_> = root.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Line comment").collect();
    assert_eq!(root_items.len(), 1);
    let f = file.symbols.iter().find(|symbol| symbol.name == "f").unwrap();
    let items: Vec<_> = f.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Line comment").collect();
    assert_eq!(items.len(), 2, "{:?}", f.rust_contract);
    for item in items {
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "//");
        assert!(item.text.contains("end of the line") && item.text.contains("not a comment delimiter") && item.text.contains("Documentation comments"));
    }
    let documented = file.symbols.iter().find(|symbol| symbol.name == "documented").unwrap();
    assert!(!documented.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Line comment"));
}
