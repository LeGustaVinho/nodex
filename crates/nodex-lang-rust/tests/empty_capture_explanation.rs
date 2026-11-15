use nodex_lang_rust::lower_source;

#[test]
fn empty_precise_capture_explains_elided_input_lifetime_without_assuming_a_valid_body() {
    let source = "trait Trait {} fn f(x: &str) -> impl Trait + use<> { loop {} } fn captured(x: &str) -> impl Trait + use<'_> { loop {} } fn other(x: i32) -> impl Trait + use<> { loop {} }";
    let ir = lower_source("src/lib.rs", source);
    let symbol = ir.symbols.iter().find(|symbol| symbol.name == "f").unwrap();
    let items = symbol.rust_contract.as_ref().unwrap();
    let detail = items.iter().find(|item| item.group == "Empty precise capture").unwrap();
    assert_eq!(&source[detail.span.start_byte as usize..detail.span.end_byte as usize], "use<>");
    assert!(detail.text.contains("elided lifetime") && detail.text.contains("not a runtime closure capture") && detail.text.contains("can still read") && detail.text.contains("does not prove"));
    assert!(items.iter().any(|item| item.group == "Precise opaque type capture"));
    for name in ["captured", "other"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Empty precise capture"), "{name}");
    }
}
