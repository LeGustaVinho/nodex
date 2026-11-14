use nodex_lang_rust::lower_source;

#[test]
fn explains_raw_names_in_declarations_without_explaining_ordinary_names() {
    let ir = lower_source("src/lib.rs", "fn r#type() {} struct r#match; fn normal() {}");
    for name in ["r#type", "r#match"] {
        let symbol = ir.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let explanation = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Identifier").unwrap();
        assert!(explanation.text.contains("raw identifier"));
        assert!(explanation.text.contains("prefix is syntax, not part of the name"));
        assert!(explanation.text.contains(name));
    }
    let ordinary = ir.symbols.iter().find(|symbol| symbol.name == "normal").unwrap();
    assert!(!ordinary.rust_contract.as_ref().is_some_and(|items| items.iter().any(|item| item.group == "Identifier")));
}
