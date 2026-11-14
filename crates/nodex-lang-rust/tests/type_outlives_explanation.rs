use nodex_lang_rust::lower_source;

#[test]
fn explains_type_outlives_without_claiming_value_lives_that_long() {
    let source = "fn inline<'a, T: 'a>(x: T) {} fn clause<'a, T>(x: T) where T: Clone + 'a {} fn lifetime<'a: 'b, 'b>() {}";
    let ir = lower_source("src/lib.rs", source);
    for name in ["inline", "clause"] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Type lifetime requirement").unwrap();
        assert!(item.text.contains("T: 'a"));
        assert!(item.text.contains("references contained"));
        assert!(item.text.contains("dropped sooner"));
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "'a");
    }
    let symbol = ir.symbols.iter().find(|s| s.name == "lifetime").unwrap();
    assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Type lifetime requirement"));
}
