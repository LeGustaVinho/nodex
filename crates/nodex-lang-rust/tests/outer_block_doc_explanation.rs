use nodex_lang_rust::lower_source;

#[test]
fn outer_block_documentation_is_explained_on_the_item_it_documents() {
    let source = "/** Documentation */ #[inline] fn documented() {} /*** ordinary block */ fn ordinary() {} mod nested { /*! Inner docs */ fn inner() {} }";
    let ir = lower_source("src/lib.rs", source);
    let symbol = ir.symbols.iter().find(|symbol| symbol.name == "documented").unwrap();
    assert_eq!(symbol.doc.as_deref(), Some("Documentation"));
    let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Outer documentation comment").unwrap();
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "/** Documentation */");
    assert!(item.text.contains("item that follows") && item.text.contains("documentation attribute") && item.text.contains("not a runtime operation"));
    for name in ["ordinary", "inner"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Outer documentation comment"));
    }
}
