use nodex_lang_rust::lower_source;

#[test]
fn enum_variant_feature_condition_is_visible_on_variant() {
    let source = "enum E { #[cfg(feature = \"x\")] X, Y }";
    let ir = lower_source("src/lib.rs", source);
    let x = ir.symbols.iter().find(|symbol| symbol.name == "X").unwrap();
    let item = x.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Feature-gated compilation").unwrap();
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "feature = \"x\"");
    for phrase in ["If it is true", "If false", "build-time selection", "does not establish"] { assert!(item.text.contains(phrase), "missing {phrase}"); }
    let y = ir.symbols.iter().find(|symbol| symbol.name == "Y").unwrap();
    assert!(!y.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Feature-gated compilation"));
}
