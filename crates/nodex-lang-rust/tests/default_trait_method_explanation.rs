use nodex_lang_rust::lower_source;

#[test]
fn default_method_contract_explains_use_and_override_without_mislabeling_others() {
    let ir = lower_source("src/lib.rs", "trait T { fn f(&self) {} fn required(&self); } struct S; impl S { fn inherent(&self) {} }");
    let method = ir.symbols.iter().find(|s| s.name == "f").unwrap();
    let item = method.rust_contract.as_ref().unwrap().iter()
        .find(|item| item.group == "Default trait method").unwrap();
    assert!(item.text.contains("default implementation of f for trait T"));
    assert!(item.text.contains("override"));
    assert!(item.text.contains("when the method is called"));
    for name in ["required", "inherent"] {
        let symbol = ir.symbols.iter().find(|s| s.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().is_some_and(|items|
            items.iter().any(|item| item.group == "Default trait method")));
    }
}
