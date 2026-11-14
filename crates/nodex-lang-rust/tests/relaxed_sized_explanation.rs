use nodex_lang_rust::lower_source;

#[test]
fn explains_relaxed_sized_bound_in_parameter_and_where_clause() {
    let source = "fn f<T: ?Sized>(x: &T) {} fn g<T>() where T: ?Sized {} fn sized<T: Sized>() {}";
    let ir = lower_source("src/lib.rs", source);
    for name in ["f", "g"] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|i| i.group == "Relaxed Sized bound").unwrap();
        assert!(item.text.contains("relax an implicit bound"));
        assert!(item.text.contains("does not require an unsized type"));
        assert!(item.text.contains("does not prove name resolution"));
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "?Sized");
    }
    let symbol = ir.symbols.iter().find(|s| s.name == "sized").unwrap();
    assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|i| i.group == "Relaxed Sized bound"));
}
