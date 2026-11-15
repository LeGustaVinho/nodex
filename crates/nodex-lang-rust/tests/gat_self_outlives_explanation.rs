use nodex_lang_rust::lower_source;

#[test]
fn gat_self_outlives_bound_is_explained_in_the_associated_type_contract() {
    let source = "trait Lending { type Item<'a> where Self: 'a; } trait Other { type Item<'b> where Self: 'static; } trait Opposite { type Item<'a> where 'a: 'static; }";
    let file = lower_source("src/lib.rs", source);
    let lending = file.symbols.iter().find(|symbol| symbol.name == "Lending").unwrap();
    let item = file.symbols.iter().find(|symbol| symbol.name == "Item" && symbol.parent.as_deref() == Some(lending.id.as_str())).unwrap();
    let explanation = item.rust_contract.as_ref().unwrap().iter().find(|item| item.group == "Associated type lifetime requirement").unwrap_or_else(|| panic!("{:?}", item.rust_contract));
    assert_eq!(&source[explanation.span.start_byte as usize..explanation.span.end_byte as usize], "Self: 'a");
    assert!(explanation.text.contains("associated type family"));
    assert!(explanation.text.contains("references contained in Self"));
    assert!(explanation.text.contains("does not keep any particular value alive"));
    assert!(explanation.text.contains("not a runtime value"));
    for name in ["Other", "Opposite"] {
        let owner = file.symbols.iter().find(|symbol| symbol.name == name).unwrap();
        let item = file.symbols.iter().find(|symbol| symbol.name == "Item" && symbol.parent.as_deref() == Some(owner.id.as_str())).unwrap();
        assert!(!item.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Associated type lifetime requirement"));
    }
}
