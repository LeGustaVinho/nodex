use nodex_lang_rust::lower_source;
#[test]
fn trailing_parameter_comma_is_optional_and_not_an_extra_parameter() {
    let source = "fn f(x: i32,) {} fn many(x: i32, y: i32, /*comment*/ ) {} trait T { fn g(x: i32,); fn plain(x: i32); } fn call() { f(1,); }";
    let ir = lower_source("src/lib.rs", source);
    for name in ["f", "many", "g"] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        let items: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Trailing comma in parameter list").collect();
        assert_eq!(items.len(), 1, "{name}: {:?}", symbol.rust_contract);
        let item = items[0];
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], ",");
        assert!(item.text.contains("optional punctuation") && item.text.contains("does not add another parameter") && item.text.contains("not a one-element tuple"));
    }
    for name in ["plain", "call"] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Trailing comma in parameter list"));
    }
}
