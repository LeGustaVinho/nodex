use nodex_lang_rust::lower_source;

#[test]
fn unsafe_trait_explains_implementer_obligation_on_symbol() {
    let text = "unsafe trait Trusted {} trait Ordinary {} struct S;";
    let ir = lower_source("src/lib.rs", text);
    let trusted = ir.symbols.iter().find(|s| s.name == "Trusted").unwrap();
    let item = trusted.rust_contract.as_ref().unwrap().iter().find(|i| i.group == "Unsafe trait").unwrap();
    assert!(item.text.contains("implementers must uphold safety requirements"));
    assert!(item.text.contains("unsafe impl"));
    assert!(item.text.contains("cannot be determined from the keyword alone"));
    assert!(item.text.contains("does not itself make every method unsafe"));
    assert_eq!(&text[item.span.start_byte as usize..item.span.end_byte as usize], "unsafe");
    for name in ["Ordinary", "S"] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().is_some_and(|items| items.iter().any(|i| i.group == "Unsafe trait")));
    }
}
