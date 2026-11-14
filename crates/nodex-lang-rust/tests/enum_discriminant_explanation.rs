#[test]
fn explicit_discriminants_are_visible_on_enum_and_variant_symbols() {
    let ir = nodex_lang_rust::lower_source("src/lib.rs", "enum E { A = 1, B = 2, C }");
    let e = ir.symbols.iter().find(|symbol| symbol.name == "E").unwrap();
    let items = e.rust_contract.as_ref().unwrap();
    assert_eq!(items.iter().filter(|item| item.group == "Enum discriminant").count(), 2);
    assert!(items.iter().any(|item| item.text.contains("`= 1` after variant A")));
    assert!(items.iter().any(|item| item.text.contains("`= 2` after variant B")));
    let a = ir.symbols.iter().find(|symbol| symbol.name == "A").unwrap();
    assert!(a.rust_contract.as_ref().unwrap().iter().any(|item| item.text.contains("discriminant")));
    let c = ir.symbols.iter().find(|symbol| symbol.name == "C").unwrap();
    assert!(!c.rust_contract.as_ref().is_some_and(|items| items.iter().any(|item| item.group == "Enum discriminant")));
}
