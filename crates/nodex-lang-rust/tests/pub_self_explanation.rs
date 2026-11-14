use nodex_lang_rust::lower_source;

#[test]
fn pub_self_explains_module_private_visibility() {
    let ir = lower_source("src/lib.rs", "mod outer { pub(self) fn f() {} pub fn g() {} fn h() {} }");
    let f = ir.symbols.iter().find(|symbol| symbol.name == "f").unwrap();
    let explanation = f.rust_contract.as_ref().unwrap().iter()
        .find(|item| item.group == "Visibility").unwrap();
    assert!(explanation.text.contains("current module and its descendant modules"));
    assert!(explanation.text.contains("same visibility as leaving the modifier out"));
    for name in ["g", "h"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        assert!(!symbol.rust_contract.as_ref().is_some_and(|items| items.iter().any(|item| item.group == "Visibility" && item.text.contains("pub(self)"))));
    }
}
