use nodex_lang_rust::lower_source;

#[test]
fn lifetime_bounds_explain_outlives_in_inline_and_where_forms() {
    let source = "fn inline<'a: 'b, 'b>(x: &'a str) -> &'b str { x } fn via_where<'a, 'b>() where 'a: 'b {} fn plain<'a, 'b>() {}";
    let ir = lower_source("src/lib.rs", source);
    for name in ["inline", "via_where"] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        let details: Vec<_> = symbol.rust_contract.as_ref().unwrap().iter().filter(|item| item.group == "Lifetime outlives bound").collect();
        assert_eq!(details.len(), 1, "{name}: {details:?}");
        let detail = details[0];
        assert!(detail.text.contains("'a to last at least as long as 'b"));
        assert!(detail.text.contains("does not keep a value alive"));
        assert!(source[detail.span.start_byte as usize..detail.span.end_byte as usize].contains("'a: 'b"));
    }
    let plain = ir.symbols.iter().find(|s| s.name == "plain").unwrap();
    assert!(!plain.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Lifetime outlives bound"));
}
