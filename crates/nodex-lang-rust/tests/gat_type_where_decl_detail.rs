use nodex_lang_rust::lower_source;

#[test]
fn gat_where_bound_applies_to_its_own_type_parameter() {
    let source = "trait T { type Item<U> where U: Clone; type Other<U> where Self: Sized; type Simple; } struct S; impl T for S { type Item<U> = Vec<U> where U: Clone; type Other<U> = U; type Simple = (); }";
    let ir = lower_source("src/lib.rs", source);
    let candidates: Vec<_> = ir.symbols.iter().filter(|symbol| symbol.name == "Item").collect();
    let facts: Vec<_> = candidates.iter().flat_map(|symbol| symbol.rust_contract.as_ref().into_iter().flatten()).filter(|item| item.group == "Generic associated type parameter requirement").collect();
    assert_eq!(facts.len(), 1, "{:?}", candidates);
    let item = facts[0];
    assert_eq!(&source[item.span.start_byte as usize..item.span.end_byte as usize], "U: Clone");
    for phrase in ["trait T", "type Item<U>", "family of associated types", "own type parameter U", "If `Clone` resolves", "does not itself clone", "does not choose"] { assert!(item.text.contains(phrase), "{}", item.text); }
    let other = ir.symbols.iter().find(|symbol| symbol.name == "Other").unwrap();
    assert!(!other.rust_contract.as_ref().into_iter().flatten().any(|item| item.group == "Generic associated type parameter requirement"));
}
