use nodex_lang_rust::lower_source;

#[test]
fn explains_associated_type_bound_not_equality_or_ordinary_bounds() {
    let source = "fn f<T: Iterator<Item: Send>>() {} fn eq<T: Iterator<Item = u8>>() {} fn ordinary<T: Send>() {}";
    let ir = lower_source("src/lib.rs", source);
    let f = ir.symbols.iter().find(|s| s.name == "f").unwrap();
    let item = f.rust_contract.as_ref().unwrap().iter().find(|i| i.group == "Associated type bound").expect("associated type bound");
    assert!(item.text.contains("associated type Item"));
    assert!(item.text.contains("rather than setting it equal"));
    assert!(item.text.contains("If Send resolves"));
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "Item: Send");
    for name in ["eq", "ordinary"] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|i| i.group == "Associated type bound"));
    }
}
