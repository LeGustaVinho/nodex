use nodex_lang_rust::lower_source;
#[test]
fn wildcard_parameter_keeps_argument_evaluation_and_has_no_binding() {
    let source = "fn f(_: i32, _named: i32, (_, value): (i32, i32)) { let _: i32 = value; } trait T { fn g(_: String); } fn normal(x: i32) {}";
    let ir = lower_source("src/lib.rs", source);
    for name in ["f", "g"] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        let items: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Ignored function parameter").collect();
        assert_eq!(items.len(), 1, "{name}: {:?}", symbol.rust_contract);
        let item = items[0];
        assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "_");
        assert!(item.text.contains("argument of type") && item.text.contains("expression is evaluated") && item.text.contains("without giving it a name") && item.text.contains("normal argument passing and ownership rules") && item.text.contains("creates a binding"));
    }
    let normal = ir.symbols.iter().find(|s| s.name == "normal").unwrap();
    assert!(!normal.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Ignored function parameter"));
}
