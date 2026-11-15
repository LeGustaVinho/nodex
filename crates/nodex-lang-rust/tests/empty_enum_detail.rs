use nodex_lang_rust::lower_source;

#[test]
fn explains_empty_enum_without_confusing_enum_with_a_variant() {
    let source = "enum Never { /* no choices */ } enum Color { Red } fn absurd(n: Never) -> u8 { match n {} }";
    let ir = lower_source("src/lib.rs", source);
    let symbol = ir.symbols.iter().find(|symbol| symbol.name == "Never").unwrap();
    let fact = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Enum with no variants").unwrap();
    assert_eq!(&source[fact.span.start_byte as usize..fact.span.end_byte as usize], "{ /* no choices */ }");
    assert!(fact.text.contains("no valid values") && fact.text.contains("uninhabited") && fact.text.contains("empty `match`") && fact.text.contains("not the `!` type") && fact.text.contains("does not prove compilation"));
    let color = ir.symbols.iter().find(|symbol| symbol.name == "Color").unwrap();
    assert!(!color.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Enum with no variants"));
}
