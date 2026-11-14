use nodex_lang_rust::lower_source;

#[test]
fn explicit_function_lifetimes_are_explained_on_function_symbols() {
    let text = "fn identity<'a>(x: &'a str) -> &'a str { x } trait T { fn choose<'x, 'y: 'x>(&self, x: &'x str, y: &'y str) -> &'x str; } fn generic<T>(x: T) {}";
    let ir = lower_source("src/lib.rs", text);
    let identity = ir.symbols.iter().find(|s| s.name == "identity").unwrap();
    let items: Vec<_> = identity.rust_contract.as_ref().unwrap().iter().filter(|i| i.group == "Lifetime parameter").collect();
    assert_eq!(items.len(), 1);
    assert_eq!(&text[items[0].span.start_byte as usize..items[0].span.end_byte as usize], "'a");
    assert!(items[0].text.contains("does not extend"));
    let choose = ir.symbols.iter().find(|s| s.name == "choose").unwrap();
    assert_eq!(choose.rust_contract.as_ref().unwrap().iter().filter(|i| i.group == "Lifetime parameter").count(), 2);
    let generic = ir.symbols.iter().find(|s| s.name == "generic").unwrap();
    assert!(!generic.rust_contract.as_ref().into_iter().flatten().any(|i| i.group == "Lifetime parameter"));
}
