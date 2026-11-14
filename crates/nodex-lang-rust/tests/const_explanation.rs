use nodex_lang_rust::lower_source;

#[test]
fn constant_declaration_is_explained_in_the_symbol_contract() {
    let ir = lower_source("src/lib.rs", "const N: usize = 10;\nstatic S: usize = 10;\n");
    let constant = ir.symbols.iter().find(|symbol| symbol.name == "N").unwrap();
    let explanation = constant.rust_contract.as_ref().unwrap().iter()
        .find(|item| item.group == "Constant").unwrap();
    assert!(explanation.text.contains("N names a constant of type usize"));
    assert!(explanation.text.contains("initializer 10 is evaluated at compile time"));
    assert!(explanation.text.contains("cannot be reassigned"));
    assert!(explanation.text.contains("not necessarily one shared memory location"));
    assert_eq!(explanation.type_info.as_ref().unwrap().native.as_deref(), Some("usize"));
    let static_item = ir.symbols.iter().find(|symbol| symbol.name == "S").unwrap();
    assert!(!static_item.rust_contract.as_ref().is_some_and(|items| items.iter().any(|item| item.group == "Constant")));
}
