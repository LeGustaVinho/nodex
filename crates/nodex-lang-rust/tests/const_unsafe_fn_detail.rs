use nodex_lang_rust::lower_source;

#[test]
fn const_unsafe_function_explains_both_obligations() {
    let source = "const unsafe fn f() {} const fn only_const() {} unsafe fn only_unsafe() {} fn ordinary() {}";
    let file = lower_source("src/lib.rs", source);
    let symbol = file.symbols.iter().find(|symbol| symbol.name == "f").unwrap();
    let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Const unsafe function").unwrap_or_else(|| panic!("{:?}", symbol.rust_contract));
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "const unsafe");
    assert!(item.text.contains("constant context"));
    assert!(item.text.contains("may run at runtime"));
    assert!(item.text.contains("caller must use an unsafe context"));
    assert!(item.text.contains("including when calling it in a constant context"));
    assert!(item.text.contains("does not prove"));
    assert!(symbol.rust_contract.as_ref().unwrap().iter().any(|item| item.group == "Const function"));
    assert!(symbol.rust_contract.as_ref().unwrap().iter().any(|item| item.group == "Unsafe function"));
    for name in ["only_const", "only_unsafe", "ordinary"] {
        let symbol = file.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Const unsafe function"));
    }
}
