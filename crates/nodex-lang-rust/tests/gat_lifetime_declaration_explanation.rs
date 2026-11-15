use nodex_lang_rust::lower_source;

#[test]
fn explains_lifetime_generic_associated_type_without_claiming_it_is_a_borrow() {
    let source = "trait T { type Item<'a>; type Plain; } struct S; impl T for S { type Item<'a> = i32; type Plain = i32; }";
    let ir = lower_source("src/lib.rs", source);
    let items: Vec<_> = ir.symbols.iter().filter(|symbol| symbol.name == "Item").collect();
    let facts: Vec<_> = items.iter().flat_map(|symbol| symbol.rust_contract.as_ref().into_iter().flatten()).filter(|item| item.group == "Lifetime-generic associated type").collect();
    assert_eq!(facts.len(), 1, "{items:?}");
    assert!(items.iter().flat_map(|symbol| symbol.rust_contract.as_ref().into_iter().flatten()).all(|item| item.group != "Associated type syntax"), "generic associated types must not receive the non-generic example");
    let fact = facts[0];
    assert_eq!(&source[fact.span.start_byte as usize..fact.span.end_byte as usize], "<'a>");
    assert!(fact.text.contains("type Item<'a>") && fact.text.contains("family of associated types") && fact.text.contains("does not prove") && fact.text.contains("where-clause"));
    let plain = ir.symbols.iter().find(|symbol| symbol.name == "Plain" && symbol.rust_contract.as_ref().is_some_and(|c| c.iter().any(|i| i.group == "Associated type syntax"))).unwrap();
    assert!(!plain.rust_contract.as_ref().unwrap().iter().any(|item| item.group == "Lifetime-generic associated type"));
}
