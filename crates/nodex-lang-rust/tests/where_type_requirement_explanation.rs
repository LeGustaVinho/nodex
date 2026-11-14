use nodex_lang_rust::lower_source;

#[test]
fn explains_where_type_bounds_in_function_and_type_contracts() {
    let text = "fn duplicate<T>(x: T) where T: Clone { } struct Bag<T> where T: Clone { value: T } fn plain<T>(x: T) {}";
    let ir = lower_source("src/lib.rs", text);
    for name in ["duplicate", "Bag"] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|i| i.group == "Where-clause type requirement").unwrap();
        assert!(item.text.contains("T must satisfy"));
        assert!(item.text.contains("If Clone resolves"));
        assert!(item.text.contains("does not call"));
        assert_eq!(&text[item.span.start_byte as usize..item.span.end_byte as usize], "T: Clone");
    }
    let plain = ir.symbols.iter().find(|s| s.name == "plain").unwrap();
    assert!(!plain.rust_contract.as_ref().into_iter().flatten().any(|i| i.group == "Where-clause type requirement"));
}
