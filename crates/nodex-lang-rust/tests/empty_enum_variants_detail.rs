use nodex_lang_rust::lower_source;

#[test]
fn three_fieldless_variant_forms_are_distinguished_on_enum_and_variants() {
    let source = "enum E { Unit, Tuple(), Record {} } enum Different { Unit, Tuple(u32), Record {} }";
    let file = lower_source("src/lib.rs", source);
    let e = file.symbols.iter().find(|symbol| symbol.name == "E").unwrap();
    for symbol in std::iter::once(e).chain(file.symbols.iter().filter(|symbol| symbol.parent.as_deref() == Some(e.id.as_str()))) {
        let item = symbol.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Three fieldless enum variant forms").unwrap_or_else(|| panic!("{}: {:?}", symbol.name, symbol.rust_contract));
        assert!(item.text.contains("E::Unit"));
        assert!(item.text.contains("E::Tuple()"));
        assert!(item.text.contains("E::Record {}"));
        assert!(item.text.contains("carry no fields"));
        assert!(item.text.contains("distinct alternatives"));
    }
    let other = file.symbols.iter().find(|symbol| symbol.name == "Different").unwrap();
    assert!(!other.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Three fieldless enum variant forms"));
}
