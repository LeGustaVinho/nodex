use nodex_lang_rust::lower_source;

#[test]
fn lifetime_generic_associated_type_and_its_self_bound_are_both_visible() {
    let source = "trait T { type Item<'a> where Self: 'a; }";
    let ir = lower_source("src/lib.rs", source);
    let trait_symbol = ir.symbols.iter().find(|symbol| symbol.name == "T").unwrap();
    let symbol = ir.symbols.iter().find(|symbol| symbol.name == "Item" && symbol.parent.as_deref() == Some(trait_symbol.id.as_str())).unwrap();
    let contract = symbol.rust_contract.as_ref().unwrap();
    let family = contract.iter().find(|item| item.group == "Lifetime-generic associated type").unwrap();
    let bound = contract.iter().find(|item| item.group == "Associated type lifetime requirement").unwrap();
    assert_eq!(&source[family.span.start_byte as usize..family.span.end_byte as usize], "<'a>");
    assert_eq!(&source[bound.span.start_byte as usize..bound.span.end_byte as usize], "Self: 'a");
    for phrase in ["family of associated types", "lifetime parameter", "does not prove", "borrows"] {
        assert!(family.text.contains(phrase), "{}", family.text);
    }
    for phrase in ["implementing type to outlive", "must hold", "does not keep any particular value alive", "not a runtime value"] {
        assert!(bound.text.contains(phrase), "{}", bound.text);
    }
}
