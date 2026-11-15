use nodex_lang_rust::lower_source;

#[test]
fn explains_local_function_scope_and_capture_limit() {
    let source = "fn outer() { fn inner() {} inner(); } fn other() {}";
    let file = lower_source("src/lib.rs", source);
    let outer = file.symbols.iter().find(|symbol| symbol.name == "outer").unwrap();
    let items: Vec<_> = outer.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Local function item").collect();
    assert_eq!(items.len(), 1, "{:?}", outer.rust_contract);
    let item = items[0];
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "inner");
    assert!(item.text.contains("enclosing block") && item.text.contains("cannot capture local variables") && item.text.contains("pass needed values") && item.text.contains("declaring it does not execute"));
    let other = file.symbols.iter().find(|symbol| symbol.name == "other").unwrap();
    assert!(!other.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Local function item"));
}
