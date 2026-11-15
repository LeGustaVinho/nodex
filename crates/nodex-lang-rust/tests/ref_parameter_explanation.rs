use nodex_lang_rust::lower_source;

#[test]
fn ref_pattern_on_reference_parameter_explains_the_extra_reference() {
    let source = "fn f<T>(ref x: &T) { let _ = x; } fn ordinary<T>(x: &T) {} fn value(ref x: u32) {} fn mutable<T>(ref mut x: &T) {}";
    let ir = lower_source("src/lib.rs", source);
    let function = ir.symbols.iter().find(|symbol| symbol.name == "f").unwrap();
    let item = function.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Reference-binding parameter").unwrap_or_else(|| panic!("{:?}", function.rust_contract));
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "ref");
    assert!(item.text.contains("already a shared reference") && item.text.contains("reference to a reference") && item.text.contains("&&T"));
    assert!(item.text.contains("`*x`") && item.text.contains("`**x`") && item.text.contains("does not clone"));
    for name in ["ordinary", "value", "mutable"] {
        let function = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!function.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Reference-binding parameter"));
    }
}
