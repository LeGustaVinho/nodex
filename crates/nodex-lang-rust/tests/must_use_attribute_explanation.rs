use nodex_lang_rust::lower_source;

#[test]
fn must_use_explains_functions_types_and_traits_without_guessing_cfg_attr() {
    let source = "#[must_use] fn make() -> i32 { 1 } #[must_use = \"consume me\"] struct Token; #[must_use] trait Task { #[must_use] fn poll(&self); } trait Other { fn trait_impl_poll(&self); } impl Other for Token { #[must_use] fn trait_impl_poll(&self) {} } #[cfg_attr(feature = \"x\", must_use)] fn conditional() {} #[must_use] const BAD: u8 = 0;";
    let ir = lower_source("src/lib.rs", source);
    for name in ["make", "Token", "Task", "poll"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Must-use diagnostic").unwrap_or_else(|| panic!("{name}: {:?}", symbol.rust_contract));
        assert!((&source[item.span.start_byte as usize..item.span.end_byte as usize]).starts_with("#[must_use"));
        assert!(item.text.contains("unused_must_use"));
        assert!(item.text.contains("lint settings can allow, warn, or deny"));
        assert!(item.text.contains("not proof that the program handled"));
        if name == "Token" { assert!(item.text.contains("consume me")); assert!(item.text.contains("value of this type")); }
        if name == "Task" { assert!(item.text.contains("opaque impl Trait")); }
    }
    let implementation = ir.symbols.iter().find(|symbol| symbol.name == "trait_impl_poll" && symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Must-use diagnostic")).unwrap();
    let item = implementation.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Must-use diagnostic").unwrap();
    assert!(item.text.contains("this attribute has no effect"));
    assert!(item.text.contains("trait's method declaration"));
    for name in ["conditional", "BAD"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Must-use diagnostic"), "{name}: {:?}", symbol.rust_contract);
    }
}
